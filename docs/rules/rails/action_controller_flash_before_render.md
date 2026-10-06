# Rails/ActionControllerFlashBeforeRender

Use `flash.now` instead of `flash` before `render`.

| | |
| --- | --- |
| Department | Rails |
| Enabled by default | false |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

Using `flash` assignment before `render` in Rails controllers will persist the message for too long. Check https://guides.rubyonrails.org/action_controller_overview.html#flash-now

This cop's autocorrection is unsafe because it changes how the message is persisted.

```ruby
# bad
class HomeController < ApplicationController
  def create
    flash[:alert] = "msg"
    render :index
  end
end

# good
class HomeController < ApplicationController
  def create
    flash.now[:alert] = "msg"
    render :index
  end
end
```

## Options

This rule has no options.

## Blind spots

A file consisting of a single top-level `flash[...] = ...` statement is never reported (RuboCop raises on it).
