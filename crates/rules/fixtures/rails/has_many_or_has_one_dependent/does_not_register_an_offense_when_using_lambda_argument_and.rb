class User < ApplicationRecord
  has_many :articles, -> { where(active: true) }, dependent: :destroy
end
