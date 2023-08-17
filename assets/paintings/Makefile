resize:
	mkdir -p ../../client/paintings
	mogrify -resize 512x512 -format webp -quality 75 -path ../../client/paintings *.png
	rm ../../client/paintings/Runway.webp
	#optipng -o7 ../../client/paintings/*.png
