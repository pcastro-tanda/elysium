class Person < ApplicationRecord
  with_options dependent: :destroy do
    has_one :foo, class_name: 'Foo'
  end
end
