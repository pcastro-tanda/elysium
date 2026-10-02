class Post < ApplicationRecord
  belongs_to :foo, -> { active }
  belongs_to :bar
  belongs_to :blah
end
