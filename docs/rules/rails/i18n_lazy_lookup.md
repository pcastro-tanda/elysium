# Rails/I18nLazyLookup

Checks for places where I18n "lazy" lookup can be used.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for places where I18n "lazy" lookup can be used.

This cop has two different enforcement modes. When the EnforcedStyle is `lazy` (the default), explicit lookups are added as offenses.

When the EnforcedStyle is `explicit` then lazy lookups are added as offenses.

```ruby
# bad (lazy)
class BooksController < ApplicationController
  def create
    redirect_to books_url, notice: t('books.create.success')
  end
end

# good (lazy)
class BooksController < ApplicationController
  def create
    redirect_to books_url, notice: t('.success')
  end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `lazy` | `lazy`, `explicit` | `lazy` flags explicit lookups, `explicit` flags lazy lookups. |

## Blind spots

None recorded.
