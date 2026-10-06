module Foo
  def self.included(base)
    base.has_many :bazs
         ^^^^^^^^ Specify a `:dependent` option.
  end
end
