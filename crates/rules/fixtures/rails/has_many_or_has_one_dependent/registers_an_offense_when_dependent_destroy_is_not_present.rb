class Person < ApplicationRecord
  with_options through: nil do
    has_many :foo
    ^^^^^^^^ Specify a `:dependent` option.
  end
end
