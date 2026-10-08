validates_length_of [:full_name, :birth_date].freeze
^^^^^^^^^^^^^^^^^^^ Prefer the new style validations `validates :column, length: value` over `validates_length_of`.
