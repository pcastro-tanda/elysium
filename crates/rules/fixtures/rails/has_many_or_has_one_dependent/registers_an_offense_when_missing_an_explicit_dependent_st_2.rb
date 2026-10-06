class Person < ApplicationRecord
  has_many :foo, class_name: 'bar'
  ^^^^^^^^ Specify a `:dependent` option.
end
