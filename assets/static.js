<script>
  const API = {
    brightness: (val) => `/api/display/brightness/${val}`,
    power: (val = 'toggle') => `/api/power/state/${val}`,
    display: (val = 'toggle') => `/api/display/state/${val}`,
    micVolume: (val) => `/api/mic/volume/${val}`,
    micMute: (val = 'toggle') => `/api/mic/mute/${val}`,
    speakerVolume: (val) => `/api/speaker/volume/${val}`,
    speakerMute: (val = 'toggle') => `/api/speaker/mute/${val}`,
    record: (val = 'start') => `/api/voice/state/${val}`,
    update: '/api/update',
    media: (action) => `/api/media/${action}`
  };

  async function callApi(url, method = 'GET') {
    try {
      const res = await fetch(url, { method });
      const text = await res.text();
      console.log(`API ${url} -> ${text}`);
      return text;
    } catch (err) { console.error(`API error ${url}`, err); }
  }

  const brightnessSlider = document.getElementById('brightnessSlider');
  const brightnessVal = document.getElementById('brightnessVal');
  brightnessSlider.addEventListener('input', (e) => {
    let v = e.target.value;
    brightnessVal.innerText = v + '%';
    callApi(API.brightness(v));
  });

  document.getElementById('togglePowerBtn').addEventListener('click', () => callApi(API.power('toggle')));
  document.getElementById('displayOnOffBtn').addEventListener('click', () => callApi(API.display('toggle')));
  document.getElementById('screensaverBtn').addEventListener('click', () => console.log('Screensaver triggered'));

  const micSlider = document.getElementById('micSlider');
  const micVolVal = document.getElementById('micVolVal');
  micSlider.addEventListener('input', (e) => {
    let v = e.target.value;
    micVolVal.innerText = v + '%';
    callApi(API.micVolume(v));
  });
  document.getElementById('micMuteBtn').addEventListener('click', () => callApi(API.micMute('toggle')));

  const speakerSlider = document.getElementById('speakerSlider');
  const speakerVolVal = document.getElementById('speakerVolVal');
  speakerSlider.addEventListener('input', (e) => {
    let v = e.target.value;
    speakerVolVal.innerText = v + '%';
    callApi(API.speakerVolume(v));
  });
  document.getElementById('speakerMuteBtn').addEventListener('click', () => callApi(API.speakerMute('toggle')));

  document.getElementById('recordBtn').addEventListener('click', () => callApi(API.record('start')));
  document.getElementById('updateBtn').addEventListener('click', () => callApi(API.update));

  document.getElementById('mediaPrev').addEventListener('click', () => callApi(API.media('prev')));
  document.getElementById('mediaPlayPause').addEventListener('click', () => callApi(API.media('playpause')));
  document.getElementById('mediaStop').addEventListener('click', () => callApi(API.media('stop')));
  document.getElementById('mediaNext').addEventListener('click', () => callApi(API.media('next')));

  function updateTime() {
    document.getElementById('liveTime').innerText = new Date().toLocaleTimeString();
  }
  setInterval(updateTime, 1000);
  updateTime();

  function randomizeTelemetry() {
    document.getElementById('battVoltage').innerText = (3.7 + Math.random() * 0.5).toFixed(2) + ' V';
    document.getElementById('battPercent').innerText = Math.floor(40 + Math.random() * 60) + ' %';
    document.getElementById('temperature').innerText = (18 + Math.random() * 12).toFixed(1) + ' °C';
    document.getElementById('humidity').innerText = Math.floor(30 + Math.random() * 45) + ' %';
    document.getElementById('rssi').innerText = -Math.floor(30 + Math.random() * 55) + ' dBm';
    document.getElementById('occupancy').innerHTML = Math.random() > 0.8 ? '👤 DETECTED' : '🌿 CLEAR';
    if (Math.random() < 0.2) document.getElementById('irStatus').innerText = '0x' + Math.floor(Math.random()*65535).toString(16);
    else document.getElementById('irStatus').innerText = '—';
  }
  setInterval(randomizeTelemetry, 7000);
  randomizeTelemetry();
</script>

