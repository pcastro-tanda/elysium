class Person < ApplicationRecord
  has_one :foo, through: nil
  ^^^^^^^ Specify a `:dependent` option.
end
