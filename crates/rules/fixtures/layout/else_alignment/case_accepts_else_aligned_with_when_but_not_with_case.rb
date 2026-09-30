case code_type
  when 'ruby', 'sql', 'plain'
    code_type
  when 'erb'
    'ruby; html-script: true'
  when "html"
    'xml'
  else
    'plain'
end
