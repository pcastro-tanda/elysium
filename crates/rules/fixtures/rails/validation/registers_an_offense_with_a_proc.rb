validates_comparison_of :a, :b, greater_than: -> { Time.zone.today }
^^^^^^^^^^^^^^^^^^^^^^^ Prefer the new style validations `validates :column, comparison: value` over `validates_comparison_of`.
