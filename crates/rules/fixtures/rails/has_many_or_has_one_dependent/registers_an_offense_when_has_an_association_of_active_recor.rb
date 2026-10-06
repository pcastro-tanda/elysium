module Foo
  extend ActiveSupport::Concern

  included do
    has_many :bazs
    ^^^^^^^^ Specify a `:dependent` option.
  end
end
