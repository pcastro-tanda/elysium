# Lint/Debugger

Checks for debugger calls.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks for debug calls (such as `debugger` or `binding.pry`) that should not be kept for production code.

The cop can be configured using `DebuggerMethods`. By default, a number of gems' debug entrypoints are configured (`Kernel`, `Byebug`, `Capybara`, `debug.rb`, `Pry`, `Rails`, `RubyJard`, and `WebConsole`). A specific default group can be disabled by setting it to `~`/`false`, and additional methods can be added under a new group name.

Gems that start a debugging session as a side effect of a bare `require` (such as `require 'debug/start'`) are configured the same way through `DebuggerRequires`.

```ruby
# bad
def some_method
  binding.pry
  do_something
end

# bad
def some_method
  byebug
  do_something
end

# good
def some_method
  do_something
end
```


## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| DebuggerMethods | `nil` |  | Debugger entry-point method chains, grouped by gem; a group set to `~`/`false` is removed, and a new group name adds its methods. Defaults (flattened): binding.irb, Kernel.binding.irb, byebug, remote_byebug, Kernel.byebug, Kernel.remote_byebug, page.save_and_open_page, page.save_and_open_screenshot, page.save_page, page.save_screenshot, save_and_open_page, save_and_open_screenshot, save_page, save_screenshot, binding.b, binding.break, Kernel.binding.b, Kernel.binding.break, binding.pry, binding.remote_pry, binding.pry_remote, Kernel.binding.pry, Kernel.binding.remote_pry, Kernel.binding.pry_remote, Pry.rescue, pry, debugger, Kernel.debugger, jard, binding.console. |
| DebuggerRequires | `nil` |  | Bare `require` arguments that start a debugging session as a side effect, grouped the same way as `DebuggerMethods`. Defaults (flattened): debug/open, debug/start. |

## Blind spots

`assumed_usage_context?`'s ancestor scan only recognises Prism's own `NodeKind::BlockNode`/`BeginNode`/`LambdaNode`; RuboCop's whitequark-only `numblock`/`itblock` distinction never arises since Prism represents every block body (named params, `_1`, or `it`) with one `BlockNode` kind.
