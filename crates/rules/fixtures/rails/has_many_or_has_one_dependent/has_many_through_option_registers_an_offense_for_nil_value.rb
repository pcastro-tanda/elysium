class Person < ApplicationRecord
  has_many :foo, through: nil
  ^^^^^^^^ Specify a `:dependent` option.
end
