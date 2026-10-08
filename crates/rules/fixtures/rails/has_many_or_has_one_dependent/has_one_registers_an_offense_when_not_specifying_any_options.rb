class Person < ApplicationRecord
  has_one :foo
  ^^^^^^^ Specify a `:dependent` option.
end
