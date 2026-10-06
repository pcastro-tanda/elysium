class Person < ActiveRecord::Base
  has_one :foo
  ^^^^^^^ Specify a `:dependent` option.
end
