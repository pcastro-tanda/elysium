class Post < ApplicationRecord
  scope :with_visibility, ->(value) { where(visible: value) }

  scope :visible, -> { with_visibility(true) }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Multiple scopes share this same expression.
  scope :hidden, -> { with_visibility(true) }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Multiple scopes share this same expression.
end
