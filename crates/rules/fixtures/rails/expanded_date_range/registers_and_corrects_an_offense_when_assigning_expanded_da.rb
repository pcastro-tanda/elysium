foo_at = date.beginning_of_day..date.end_of_day
         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `date.all_day` instead.
Model.find_by(foo_at: foo_at)
