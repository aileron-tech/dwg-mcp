# dwg-mcp roadmap

## Hosted service

- [ ] Serve the authenticated Streamable HTTP endpoint at
  `https://cad.aileron-tech.com/mcp`, with
  `https://cad.aileron-tech.fr/mcp` as the secondary address. Verify TLS and run a
  public DWG, DXF and SVG through the server from outside the host.
- [ ] Add individual accounts, revocable API keys for compatible MCP clients,
  and OAuth for marketplace clients. Isolate cached results, artifacts and
  usage by account. Keep the local stdio installation available.
- [ ] Complete the OpenAI domain verification and reviewer test flow. Track
  the separate Claude Marketplace submission.

## Prepaid drawing packs

Build the checkout, account dashboard and API key management in
`dwg-mcp-web`. The direct hosted service uses the published Apify Actor's unit
rates. At $1 per 1,000 prepaid usage units, one fresh parse uses 7 units and
one eligible cache hit uses 3. [Current Apify pricing](https://apify.com/aileron-tech/dwg-apify/pricing)

| Usage | Units | USD per drawing | USD per 1,000 drawings |
| --- | ---: | ---: | ---: |
| Fresh successful parse | 7 | $0.007 | $7.00 |
| Eligible same-account cache hit | 3 | $0.003 | $3.00 |
| Failed parse, job check, or result read | 0 | $0.00 | $0.00 |

- [ ] Let customers buy a usage pack, see their balance and payment history,
  and create or revoke named API keys. Starter includes five successful
  parses. The bilingual site and account console use Ory Kratos. Decide
  production pack sizes and the paid usage expiry policy before launch.
- [ ] Evaluate Paddle as merchant of record. Confirm it accepts these
  account-bound drawing packs before enabling live sales. Grant units once
  per completed payment through a verified webhook. [Paddle one-time payments](https://developer.paddle.com/get-started/how-paddle-works/digital-products/) and [Paddle acceptable use policy](https://www.paddle.com/help/start/intro-to-paddle/what-am-i-not-allowed-to-sell-on-paddle)
- [ ] Reserve units before processing and settle the actual charge once per
  completed job. Release the reservation on failure. Verify checkout,
  concurrent jobs, low balances, refunds and duplicate webhooks in a sandbox.
  Track optional automatic top-ups separately.
- [ ] If the Apify Actor uses the hosted MCP, provision a dedicated, revocable
  service key with an explicit high quota. Attribute its usage separately.
  Apify charges that channel's customers without debiting their direct packs.
- [ ] Let ChatGPT users sign in to an existing account and access its
  entitlements. Keep purchase and upgrade prompts out of the ChatGPT plugin.
  [OpenAI plugin guidelines](https://developers.openai.com/plugins/plugin-guidelines)

## Directory distribution

- [ ] Publish the canonical server metadata and verified installation
  instructions in the [official MCP Registry](https://registry.modelcontextprotocol.io/).
- [ ] Check discoverability in the [GitHub MCP Registry](https://github.com/mcp)
  after official registry publication.
- [ ] List the open-source server on Glama and verify the hosted connector
  appears after its official registry entry is imported. [Glama submission guide](https://glama.ai/mcp/faq)
- [ ] Open an entry for the public repository in
  [punkpeye/awesome-mcp-servers](https://github.com/punkpeye/awesome-mcp-servers).
- [ ] Submit to [MCP Market](https://mcpmarket.com/submit) and record the public
  listing URL after review.
- [ ] Submit to [MCPServers.org](https://mcpservers.org/submit) and record the
  public listing URL after review.
