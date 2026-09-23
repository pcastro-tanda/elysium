new(table_name,
    exec_query("info('#{row['name']}')").map { |col|
      col['name']
    })
