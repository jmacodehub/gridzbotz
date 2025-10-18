const { HermesClient } = require('@pythnetwork/hermes-client');
const express = require('express');

const app = express();
const hermes = new HermesClient('https://hermes.pyth.network');

const PRICE_IDS = [
  '0xef0d8b6fda2ceba41da39a73436148de22aeb0b51deb47e5f6bdc5caf5bcb3d4', // SOL/USD
  '0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43', // BTC/USD
  '0xff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace', // ETH/USD
];

app.get('/prices', async (req, res) => {
  try {
    const updates = await hermes.getLatestPriceUpdates(PRICE_IDS);
    const parsed = updates.parsed.map(p => ({
      feed_id: p.id,
      price: Number(p.price.price) * Math.pow(10, p.price.expo),
      confidence: Number(p.price.conf) * Math.pow(10, p.price.expo),
      publish_time: p.price.publish_time,
    }));
    res.json({ prices: parsed });
  } catch (e) {
    res.status(500).json({ error: e.message });
  }
});

app.listen(8080, () => console.log('Pyth proxy on http://localhost:8080/prices'));

