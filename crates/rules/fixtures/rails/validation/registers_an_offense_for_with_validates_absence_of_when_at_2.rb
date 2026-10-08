validates_absence_of [:full_name, :birth_date].freeze
^^^^^^^^^^^^^^^^^^^^ Prefer the new style validations `validates :column, absence: value` over `validates_absence_of`.
