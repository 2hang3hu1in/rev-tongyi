
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::sync::LazyLock;
use std::sync::Arc;
use reqwest::{cookie::Jar, Url};
use reqwest::{Client,ClientBuilder};
use serde_json::json;
use serde_json::Value;
use std::error::Error;
use std::collections::HashMap;
use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::io::Read;
use futures_util::StreamExt;

/// 从文件中读取cookies，读取name和value保存到hashmap
 static COOKIES_JSON: LazyLock<Arc<HashMap<String, String>>> = LazyLock::new(|| {
    let mut file = File::open("cfg/cookies.json").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let v: Value = serde_json::from_str(&contents).unwrap();
    let mut hash_map = HashMap::new();
    for v in v.as_array().unwrap().iter() {
        hash_map.insert(v["name"].to_string().replace("\"", ""), v["value"].to_string().replace("\"", ""));
    }
    Arc::new(hash_map)
});


static USER_AGENT: LazyLock<Arc<String>> = LazyLock::new(|| {
    Arc::new("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/237.84.2.178 Safari/537.36".to_string())
});

fn gen_request_id() -> String {
    let uuid = uuid::Uuid::new_v4();
    uuid.to_string()
}

#[derive(Debug)]
pub struct Chatbot {
  
    api_base: String ,

    /// Current session id
    session_id: String,

    /// Parent msg id
    parent_id: String,

    client : Client,

    x_xsrf_token: String

}

impl  Chatbot {
  pub  fn new(hashmap: Arc<HashMap<String, String>>) -> Self { 
        let x_xsrf_token = hashmap["XSRF-TOKEN"].as_str();
        let api_base = "https://qianwen.biz.aliyun.com/dialog";
        let url = api_base.parse::<Url>().unwrap();
        // 创建一个 Cookie Jar
        let cookie_jar = Arc::new(Jar::default());
          // 将 HashMap 中的 Cookies 转换为 `Set-Cookie` 格式并添加到 Jar
        for (key, value) in COOKIES_JSON.iter() {
            let cookie_str = format!("{}={}", key, value);
            cookie_jar.add_cookie_str(&cookie_str, &url);
        }
        let client = ClientBuilder::new()
        .cookie_provider(Arc::clone(&cookie_jar))
        .build().unwrap();
        Self {
            api_base: api_base.to_string(),
            session_id: "".to_string(),
            parent_id: "0".to_string(),
            client:client,
            x_xsrf_token:  x_xsrf_token.to_string()
        }
    }
    pub async fn async_non_stream_ask(& mut self,prompt: &str,parent_id: &str,session_id: &str)->Result<PromptJar,Box<dyn Error>>{
        if !parent_id.is_empty() {
            self.parent_id = parent_id.to_string();
        }
        if !session_id.is_empty(){
            self.session_id = session_id.to_string()
        }
        let user_agent = USER_AGENT.as_str();
       // let x_xsrf_token = self.cookies["XSRF-TOKEN"].as_str();

        let  headers: HeaderMap = get_http_headers(user_agent,self.x_xsrf_token.as_str());
       

        let data =  json!({
            "action": "next",
            "contents": [
                {
                    "contentType": "text",
                    "content": prompt,
                    "role": "user"
                }
            ],
            "mode": "chat",
            "model": "",
            "requestId": gen_request_id(),
            "parentMsgId": self.parent_id,
            "sessionId": self.session_id,
            "sessionType": "text_chat",
            "userAction": "chat"
    });

    // 将 `data` 序列化为 JSON 字符串
    let serialized_data = serde_json::to_string(&data)?;
    let to_url = format!("{}/conversation",self.api_base);
    // 发送 POST 请求
    let response = self.client
        .post(to_url)
        .headers(headers)
        .body(serialized_data)
        .send()
        .await?;

      // 检查响应状态
    if !response.status().is_success() {
        eprintln!("Error: {:?}", response.status());
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "HTTP request failed",
        )));
    }
    let mut res = String::new();
    let mut msg_id = String::new();
    let mut parent_msg_id = String::new();
    // let mut vec_str = Vec::new();
    // 读取分块响应体
    let mut stream = response.bytes_stream(); // 获取响应体作为流
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                let chunk_str = String::from_utf8_lossy(&bytes);
                if chunk_str.starts_with("data:") &&  chunk_str.ends_with("}\n\n"){
                    let json_str = chunk_str.strip_prefix("data:").unwrap().strip_suffix("\n\n").unwrap();
                    let message: serde_json::Result<Message> = serde_json::from_str(json_str);
                    match message {
                        Ok(message) => {
                            self.session_id = message.session_id;
                            self.parent_id = message.msg_id;
                            parent_msg_id = message.parent_msg_id;
                            msg_id = self.parent_id.clone();
                            res.clear();
                            match message.contents {
                                Some(contents) => {
                                    for content in contents {
                                        if content.content_type == "text" {
                                            res.push_str(&content.content);
                                        }
                                    }
                                }
                                None => {
                                    continue;
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("Error parsing JSON: {}", err);
                        }
                    }
                }
            }
            Err(err) => {
                eprintln!("Error reading chunk: {}", err);
                break;
            }
        }
    }
  
    

    Ok(PromptJar {
        prompt: prompt.to_string(),
        session_id: self.session_id.clone(),
        parent_id: parent_msg_id,
        msg_id: msg_id,
        content: res
    })
    
    }
     
}





fn get_http_headers<'a>(user_agent: &'a str, x_xsrf_token: &'a str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("accept"),
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(
        HeaderName::from_static("accept-encoding"),
        HeaderValue::from_static("gzip, deflate, br"),
    );
    headers.insert(
        HeaderName::from_static("accept-language"),
        HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6"),
    );
    headers.insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        HeaderName::from_static("origin"),
        HeaderValue::from_static("https://tongyi.aliyun.com"),
    );
    headers.insert(
        HeaderName::from_static("referer"),
        HeaderValue::from_static("https://tongyi.aliyun.com/"),
    );
    headers.insert(
        HeaderName::from_static("user-agent"),
        HeaderValue::from_str(user_agent).unwrap(),
    );
    headers.insert(
        HeaderName::from_static("x-platform"),
        HeaderValue::from_static("pc_tongyi"),
    );
    headers.insert(
        HeaderName::from_static("x-xsrf-token"),
        HeaderValue::from_str(x_xsrf_token).unwrap(),
    );

    headers
}


#[derive(Debug, Deserialize, Serialize)]
struct Message {
    #[serde(rename = "contentType")]
    content_type: String,
    #[serde(rename = "contents")]
    contents: Option< Vec<Content>>,
    #[serde(rename = "msgStatus")]
    msg_status: String,
    #[serde(rename = "msgId")]
    msg_id: String,
    #[serde(rename = "parentMsgId")]
    parent_msg_id: String,
    #[serde(rename = "sessionId")]
    session_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Content {
    #[serde(rename = "content")]
    content: String,
    #[serde(rename = "contentType")]
    content_type: String,
    #[serde(rename = "id")]
    id: String,
    #[serde(rename = "role")]
    role: String,
    #[serde(rename = "status")]
    status: String,
}

/// 问题压缩包，包含问题和答案
#[derive(Debug)]
pub struct PromptJar {
   pub prompt: String,
   pub session_id: String,
   pub parent_id: String,
   pub msg_id: String,
   pub content : String,
}



#[cfg(test)]
mod test {
    use tokio;
    use std::sync::Arc;
    use super::Chatbot;
    use super::COOKIES_JSON;

    #[test]
    fn uuid() {
        let uuid = uuid::Uuid::new_v4();
        println!("{}", uuid);
    }

    #[tokio::test]
   async fn chatbot() {
        let mut chatbot = Chatbot::new(Arc::clone(&COOKIES_JSON));
        let mut res =  chatbot.async_non_stream_ask("为什么蓝猫这么凶", "", "").await.unwrap(); 
        println!("第一个问题回答的结果是： {:?}",res);

        let mut chatbot2 = Chatbot::new(Arc::clone(&COOKIES_JSON));
        res =chatbot2.async_non_stream_ask("我刚刚问你的问题是什么", &res.msg_id, &res.session_id).await.unwrap();
        println!("最后回答的结果是： {:?}",res);
    }
}
