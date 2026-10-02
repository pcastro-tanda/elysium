# Sorbet/ForbidSuperclassConstLiteral

Forbid superclasses which are non-literal constants.

| | |
| --- | --- |
| Department | Sorbet |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | nursery |

Correct superclass `send` expressions by constant literals.

Sorbet, the static checker, is not (yet) able to support constructs on the following form:

```ruby
class Foo < send_expr; end
```

Multiple occurences of this can be found in Shopify's code base like:

```ruby
class ShopScope < Component::TrustedIdScope[ShopIdentity::ShopId]
```
or
```ruby
class ApiClientEligibility < Struct.new(:api_client, :match_results, :shop)
```

## Options

This rule has no options.

## Blind spots

None recorded.
