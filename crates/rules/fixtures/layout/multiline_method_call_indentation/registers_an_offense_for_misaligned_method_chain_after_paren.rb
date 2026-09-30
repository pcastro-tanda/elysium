def run
  (date_columns + candidate_columns).uniq
                                    .select { |column_name|
                                    ^^^^^^^ Use 2 (not 34) spaces for indenting an expression spanning multiple lines.
      castable?(column_name)
    }
    .each { |column_name|
      cast(column_name)
    }
end
