# Rails/RenderInline

Prefer using a template over inline rendering.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | none |
| Stability | stable |

Looks for inline rendering within controller actions.

```ruby
# bad
class ProductsController < ApplicationController
  def index
    render inline: "<% products.each do |p| %><p><%= p.name %></p><% end %>", type: :erb
  end
end

# good
# app/views/products/index.html.erb
# <% products.each do |p| %>
#   <p><%= p.name %></p>
# <% end %>

class ProductsController < ApplicationController
  def index
  end
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
