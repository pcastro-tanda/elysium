class Person < ActiveRecord::Base
  has_one :foo
  ^^^^^^^ Specify a `:dependent` option.

  def readonly?
    false
  end
end
