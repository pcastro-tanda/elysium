class User < ApplicationRecord
  has_many :activities, -> { order(created_at: :desc) }, through: :notes, source: :activities
end
