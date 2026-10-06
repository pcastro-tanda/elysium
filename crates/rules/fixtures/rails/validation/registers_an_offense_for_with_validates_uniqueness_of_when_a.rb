validates_uniqueness_of [:full_name, :birth_date]
^^^^^^^^^^^^^^^^^^^^^^^ Prefer the new style validations `validates :column, uniqueness: value` over `validates_uniqueness_of`.
