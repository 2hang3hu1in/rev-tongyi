# rev-tongyi
阿里通义千问 Rust 逆向API

### 如何使用

1. 安装 [Chrome](https://chromewebstore.google.com/detail/modifycookie-advanced-coo/cmlkddnmblbplnljlhgcblpidalbecbe) 或 [Firefox](https://addons.mozilla.org/en-US/firefox/addon/cookie-editor/) 上的Cookies Editor插件
2. 前往 https://qianwen.aliyun.com/ 并登录
3. 打开此插件，点击`Export`->`Export as JSON`，将复制的Cookies内容保存到文件`cfg/cookies.json`

## 通义千问 - AI对话

```rust

        let mut chatbot = Chatbot::new(Arc::clone(&COOKIES_JSON));
        let mut res =  chatbot.async_non_stream_ask("为什么蓝猫这么凶", "", "").await.unwrap(); 
        println!("第一个问题回答的结果是： {:?}",res);

        let mut chatbot2 = Chatbot::new(Arc::clone(&COOKIES_JSON));
        res =chatbot2.async_non_stream_ask("我刚刚问你的问题是什么", &res.msg_id, &res.session_id).await.unwrap();
        println!("最后回答的结果是： {:?}",res);
   

```



### 连续对话

返回值中有`msg_id`和`session_id`，下一次调用`async_non_stream_ask`时以`msg_id`和`session_id`传入这两个值，即可继续对话。








