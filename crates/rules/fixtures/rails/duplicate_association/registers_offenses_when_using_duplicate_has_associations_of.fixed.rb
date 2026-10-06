class Post < ApplicationRecord
  has_many :bars, class_name: 'Foo'

  has_one :qux, class_name: 'Bar'
end
