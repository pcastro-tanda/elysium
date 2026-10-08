class Person < ApplicationRecord
  has_one :foo, **bar
  ^^^^^^^ Specify a `:dependent` option.
end
