class Post < ApplicationRecord
  scope :visible, -> { where(visible: true) }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Multiple scopes share this same expression.
  scope :hidden, -> { where(visible: true) }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Multiple scopes share this same expression.
  scope :new, -> { where(created_at: 1.week.ago..Date.current) }
  scope :popular, -> { where(comments_count: 1000..Float::INFINITY) }
end
