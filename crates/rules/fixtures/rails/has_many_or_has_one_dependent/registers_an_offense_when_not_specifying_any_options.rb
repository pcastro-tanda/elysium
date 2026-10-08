class Person < ApplicationRecord
  has_many :foo
  ^^^^^^^^ Specify a `:dependent` option.
end
