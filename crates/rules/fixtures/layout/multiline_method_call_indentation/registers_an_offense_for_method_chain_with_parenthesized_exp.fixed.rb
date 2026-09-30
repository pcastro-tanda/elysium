def run
  (date_columns + candidate_columns).uniq
                                      .select { |column_name| castable?(column_name) }
end
