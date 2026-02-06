use rand::seq::SliceRandom;
use rand::Rng;

/// 生成随机工单标题
pub fn random_title() -> String {
    let titles: &[&str] = &[
        "香港轻量应用服务器带宽被限速，请帮忙检查解除",
        "我的香港轻量服务器网速异常，请协助处理下",
        "轻量应用服务器实际带宽远低于购买规格，请核实",
        "香港服务器带宽好像被限制了，麻烦帮看下",
        "轻量服务器下载速度变得很慢，请帮忙排查",
        "香港轻量服务器网络受限，请帮忙解除带宽限速",
        "服务器带宽不达标，下载速度远低于30Mbps",
        "我的轻量应用服务器带宽好像被限速了，请检查",
        "香港轻量服务器带宽问题咨询",
        "轻量服务器带宽异常，下载很慢请帮忙看看",
        "香港轻量应用服务器带宽严重缩水",
        "轻量服务器实际网速跟购买时差距很大",
    ];
    let mut rng = rand::thread_rng();
    titles.choose(&mut rng).unwrap().to_string()
}

/// 生成随机工单描述（包含实测速度数据，使每次内容自然不同）
pub fn random_description(speed_mbps: f64) -> String {
    let mut rng = rand::thread_rng();

    // 速度显示格式随机化
    let speed_str = if rng.gen_bool(0.5) {
        format!("{}", speed_mbps.round() as i64)
    } else {
        format!("{:.1}", speed_mbps)
    };

    let greetings: &[&str] = &["您好，", "你好，", "您好！\n", ""];
    let greeting = *greetings.choose(&mut rng).unwrap();

    let bodies: Vec<String> = vec![
        format!(
            "我购买的香港轻量应用服务器带宽为30Mbps，\
            但目前实际带宽只有约{}Mbps左右。\
            请帮忙检查服务器是否存在带宽限速情况，\
            如果存在限速请帮忙解除，恢复到购买时承诺的30Mbps带宽。",
            speed_str
        ),
        format!(
            "我的香港轻量应用服务器最近网速很慢，\
            刚测了一下下载速度只有{}Mbps，\
            我买的是30Mbps的套餐。\
            能帮我看看是不是被限速了吗？如果是的话麻烦帮忙解除一下。",
            speed_str
        ),
        format!(
            "我有一台香港的轻量应用服务器，配置的带宽是30Mbps，\
            但我刚测试了下载速度只有大概{}Mbps，感觉被限速了。\
            请帮忙检查一下，如果确实有限速的话帮忙解除。",
            speed_str
        ),
        format!(
            "我在用香港轻量应用服务器，带宽套餐是30Mbps的，\
            但是实测下载只有{}Mbps，速度明显不对。\
            麻烦帮忙查一下是不是有限速，帮忙处理一下。",
            speed_str
        ),
        format!(
            "我的香港轻量服务器30Mbps带宽，\
            现在实际下载速度只有{}Mbps，\
            跟购买时承诺的差太多了。\
            请帮忙看看是怎么回事，是否可以恢复正常带宽。",
            speed_str
        ),
        format!(
            "我发现我的香港轻量应用服务器带宽有问题。\
            购买的是30Mbps，但测速只有{}Mbps。\
            请问是被限速了吗？能否帮忙检查处理一下？",
            speed_str
        ),
        format!(
            "我购买了香港区域的轻量应用服务器，标注带宽30Mbps。\
            但是今天测试发现下载速度只有{}Mbps，\
            严重低于标称值。请帮我检查一下是否存在限速，\
            如有限速请帮忙恢复。",
            speed_str
        ),
        format!(
            "香港轻量应用服务器的带宽应该是30Mbps，\
            但我实际测试下来只有{}Mbps。\
            请问这个是什么情况？能帮忙看看吗？",
            speed_str
        ),
    ];
    let body = bodies.choose(&mut rng).unwrap();

    let endings: &[&str] = &[
        "谢谢！",
        "感谢！",
        "谢谢",
        "麻烦了，谢谢！",
        "辛苦了，谢谢！",
        "感谢帮忙！",
        "谢谢，期待回复。",
        "",
    ];
    let ending = *endings.choose(&mut rng).unwrap();

    format!("{}{}{}", greeting, body, ending)
}
