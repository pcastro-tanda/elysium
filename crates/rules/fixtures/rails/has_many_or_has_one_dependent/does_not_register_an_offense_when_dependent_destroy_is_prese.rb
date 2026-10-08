class Article < ApplicationRecord
  with_options dependent: :destroy do
    has_many :tags
    with_options class_name: 'Tag' do
      has_many :special_tags, foreign_key: :special_id, inverse_of: :special
    end
  end
end
