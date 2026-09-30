def run
  (date_columns + candidate_columns).uniq
                                    .select { |column_name| castable?(column_name) }
                                    ^^^^^^^ Indent `.select` 2 spaces more than `.uniq` on line 2.
end
