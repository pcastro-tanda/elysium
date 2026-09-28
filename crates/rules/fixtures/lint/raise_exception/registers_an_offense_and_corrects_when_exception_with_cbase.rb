module Gem
  def self.foo
    raise ::Exception
          ^^^^^^^^^^^ Use `StandardError` over `Exception`.
  end
end
