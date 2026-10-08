validates :a, :b, comparison: { greater_than: -> { Time.zone.today } }
