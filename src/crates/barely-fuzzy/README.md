# **barely-fuzzy**

 
[![Sponsors](https://img.shields.io/github/sponsors/QuackHack-McBlindy?logo=githubsponsors&label=Sponsor&style=flat&labelColor=ff1493&logoColor=fff&color=rgba(234,74,170,0.5) "")](https://github.com/sponsors/QuackHack-McBlindy) [![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-Sponsor?style=flat&logo=buymeacoffee&logoColor=fff&labelColor=ff1493&color=ff1493)](https://buymeacoffee.com/quackhackmcblindy)


Don't let the name fool you. `barely-fuzzy` is very capable of fuzzy matching even though designed for bare metal and `no_std` embedded projects.   


## **Installation**

  
Add **barely-fuzzy** as a dependency in `Cargo.toml`.

```toml
[dependencies]
barely-fuzzy = "0.1.0"
```
  


<br>

## **Example usage**


```
let candidates: &[&[u8]] = &[
    b"hello this is a fuzzy test",
    b"goodbye world",
    b"fuzzy test example",
    b"HELLO THIS IS A FUZZY TEST",
];

let (best, score) = barely_fuzzy::best_fuz(input.as_bytes(), candidates, 30);
let response = alloc::format!(
    "best: '{}', similarity: {}%",
    core::str::from_utf8(best).unwrap_or("?"),
    score
);
info(response);
```



<br><br>

## **☕**

[![Sponsors](https://img.shields.io/github/sponsors/QuackHack-McBlindy?logo=githubsponsors&label=Sponsor&style=flat&labelColor=ff1493&logoColor=fff&color=rgba(234,74,170,0.5) "")](https://github.com/sponsors/QuackHack-McBlindy) [![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-Sponsor?style=flat&logo=buymeacoffee&logoColor=fff&labelColor=ff1493&color=ff1493)](https://buymeacoffee.com/quackhackmcblindy)
> Like my work?   
> Buy me a coffee, or become a sponsor.  
> Thanks for supporting open source!    

♥️₿ *Wallet:* `pungkula.x`     
<a href="https://www.buymeacoffee.com/quackhackmcblindy" target="_blank"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee" style="height: 60px !important;width: 217px !important;" ></a>

<br><br>

## **Lisence**

**MIT**  
