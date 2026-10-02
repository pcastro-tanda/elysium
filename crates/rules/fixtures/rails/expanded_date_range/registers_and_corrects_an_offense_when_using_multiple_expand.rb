Model.where(foo_at: date1.beginning_of_week..date1.end_of_week, bar_at: date2.beginning_of_year..date2.end_of_year)
                                                                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `date2.all_year` instead.
                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `date1.all_week` instead.
