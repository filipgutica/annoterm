on run
	return
end run

on open markdownFiles
	set applicationPath to POSIX path of (path to me)
	set coreBinary to applicationPath & "Contents/Resources/bin/annoterm"

	repeat with markdownFile in markdownFiles
		set markdownPath to POSIX path of markdownFile
		my launchInTerminal(coreBinary, markdownPath)
	end repeat
end open

on launchInTerminal(coreBinary, markdownPath)
	set terminalCommand to quoted form of coreBinary & " " & quoted form of markdownPath

	try
		using terms from application "/System/Applications/Utilities/Terminal.app"
			tell application "/System/Applications/Utilities/Terminal.app"
				activate
				do script terminalCommand
			end tell
		end using terms from
	on error errorMessage number errorNumber
		if errorNumber is -1743 then
			«event sysodisA» "Annoterm needs permission to control Terminal" given «class mesS»:"Open System Settings > Privacy & Security > Automation, allow Annoterm to control Terminal, then open the document again."
		else
			«event sysodisA» "Annoterm could not open Terminal.app" given «class mesS»:errorMessage
		end if
	end try
end launchInTerminal
