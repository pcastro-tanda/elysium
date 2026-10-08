class Post < ApplicationRecord
  belongs_to :foos, class_name: 'Foo'
  belongs_to :bars, class_name: 'Foo'
end
