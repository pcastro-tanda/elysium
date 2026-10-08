validates_uniqueness_of [:full_name, :birth_date].freeze
^^^^^^^^^^^^^^^^^^^^^^^ Prefer the new style validations `validates :column, uniqueness: value` over `validates_uniqueness_of`.
