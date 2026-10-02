        case response = get("server/list")
        when server = response.take(1)
          if @size ||= server.take(:size)
            pass
          elsif @@image &&= server.take(:image)
            pass
          end
        end
