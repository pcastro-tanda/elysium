formatted_int = int_part
                .to_s
                .reverse
                .gsub(/...(?=.)/, '&_')
