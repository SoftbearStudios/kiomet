if (!('Rewarded' in window)) {
    window.Rewarded = "ads";

    console.log("simulating rewarded ads");

    function send(msg) {
        window.postMessage(msg, '*');
    }

    let first = true;
    let rewarded = false;
    window.addEventListener('message', (event) => {
        switch (event.data) {
            case "requestRewardedAd":
                if (!rewarded) {
                    setTimeout(() => {
                        send("tallyRewardedAd");
                        document.body.style.filter = "initial";
                        rewarded = false;
                    }, 1000);
                    rewarded = true;
                    document.body.style.filter = "brightness(0.75)";
                }
                break;
        }
    });
    send("snippetLoaded");
    send("enableRewardedAds");
}