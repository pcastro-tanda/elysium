class Person < ApplicationRecord
  has_one :foo, class_name: 'bar'
  ^^^^^^^ Specify a `:dependent` option.
end
