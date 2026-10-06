class Person < ActiveRecord::Base
  has_one :foo

  def readonly?
    true
  end
end
