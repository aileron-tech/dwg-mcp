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

## Billing with Stripe

- [ ] Agree on the free allowance, paid plan prices, included drawing counts
  and how to price same-account cache hits. Use CADLens as a reference for
  simple plans and free result reads. [CADLens usage and quotas](https://cadlens.co/docs#usage)
- [ ] Add Stripe Checkout and the customer portal on Aileron's site for plan
  management. Enforce account entitlements and show consumed and remaining
  allowance. [Stripe Billing](https://docs.stripe.com/billing/subscriptions/usage-based)
- [ ] Count completed parses with a durable usage record keyed to the drawing
  job. Keep failed parses, job checks and result reads free. Test checkout,
  cancellation, webhook retries, concurrent quota use and duplicate events
  in Stripe test mode before enabling live charges.
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
