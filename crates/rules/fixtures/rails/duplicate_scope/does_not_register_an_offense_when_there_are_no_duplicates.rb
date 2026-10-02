class Post < ApplicationRecord
  scope :visible, -> { where(visible: true) }
  scope :hidden, -> { where(visible: false) }
  scope :new, -> { where(created_at: 1.week.ago..Date.current) }
  scope :popular, -> { where(comments_count: 1000..Float::INFINITY) }
end
