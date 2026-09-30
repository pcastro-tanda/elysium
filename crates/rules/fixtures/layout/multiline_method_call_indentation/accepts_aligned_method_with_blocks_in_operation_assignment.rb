@comment_lines ||=
  src.comments
     .select { |c| begins_its_line?(c) }
     .map { |c| c.loc.line }
