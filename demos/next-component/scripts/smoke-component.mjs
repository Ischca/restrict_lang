import assert from "node:assert/strict";

const { pricing } = await import("../generated/restrict/restrict.js");

const memberQuote = pricing.quote(1_200, 4, true);
assert.deepEqual(memberQuote, {
  subtotal: 4_800,
  discount: 480,
  shipping: 500,
  total: 4_820,
});

const freeShippingQuote = pricing.quote(2_000, 3, false);
assert.deepEqual(freeShippingQuote, {
  subtotal: 6_000,
  discount: 0,
  shipping: 0,
  total: 6_000,
});

console.log("Restrict Component smoke passed", {
  memberQuote,
  freeShippingQuote,
});
