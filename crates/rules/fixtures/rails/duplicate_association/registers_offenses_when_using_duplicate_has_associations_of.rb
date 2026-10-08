class Post < ApplicationRecord
  has_many :foos, class_name: 'Foo'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `class_name: 'Foo'` is defined multiple times. Don't repeat associations.
  has_many :bars, class_name: 'Foo'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `class_name: 'Foo'` is defined multiple times. Don't repeat associations.

  has_one :baz, class_name: 'Bar'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `class_name: 'Bar'` is defined multiple times. Don't repeat associations.
  has_one :qux, class_name: 'Bar'
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Association `class_name: 'Bar'` is defined multiple times. Don't repeat associations.
end
